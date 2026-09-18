use std::collections::BTreeMap;

use crate::command::{CancelOrder, NewOrder, OrderCommand, OrderSide, ReplaceOrder};
use crate::order::ClientOrderId;
use crate::session::{MessageSeq, SessionError, SessionState};

const MAGIC: &[u8; 4] = b"SEX1";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 24;
const MAX_PAYLOAD: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Logon = 1,
    NewOrder = 2,
    CancelOrder = 3,
    ReplaceOrder = 4,
    ExecutionReport = 5,
    Reject = 6,
    Heartbeat = 7,
    ResendRequest = 8,
    Logout = 9,
    MarketData = 10,
    Admin = 11,
}

impl MessageType {
    fn from_u8(value: u8) -> Result<Self, ProtocolError> {
        match value {
            1 => Ok(Self::Logon),
            2 => Ok(Self::NewOrder),
            3 => Ok(Self::CancelOrder),
            4 => Ok(Self::ReplaceOrder),
            5 => Ok(Self::ExecutionReport),
            6 => Ok(Self::Reject),
            7 => Ok(Self::Heartbeat),
            8 => Ok(Self::ResendRequest),
            9 => Ok(Self::Logout),
            10 => Ok(Self::MarketData),
            11 => Ok(Self::Admin),
            _ => Err(ProtocolError::UnknownMessageType(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Trader,
    MarketMaker,
    Risk,
    Operations,
    Admin,
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    SubmitOrders,
    CancelOrders,
    ReplaceOrders,
    ViewMarketData,
    RequestRecovery,
    OperateMarket,
    OperateAccounts,
}

impl Role {
    pub const fn allows(self, permission: Permission) -> bool {
        match self {
            Self::Trader => matches!(
                permission,
                Permission::SubmitOrders
                    | Permission::CancelOrders
                    | Permission::ReplaceOrders
                    | Permission::ViewMarketData
                    | Permission::RequestRecovery
            ),
            Self::MarketMaker => matches!(
                permission,
                Permission::SubmitOrders
                    | Permission::CancelOrders
                    | Permission::ReplaceOrders
                    | Permission::ViewMarketData
                    | Permission::RequestRecovery
            ),
            Self::Risk => matches!(
                permission,
                Permission::ViewMarketData
                    | Permission::RequestRecovery
                    | Permission::OperateAccounts
            ),
            Self::Operations => matches!(
                permission,
                Permission::ViewMarketData
                    | Permission::RequestRecovery
                    | Permission::OperateMarket
                    | Permission::OperateAccounts
            ),
            Self::Admin => true,
            Self::ReadOnly => matches!(permission, Permission::ViewMarketData),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credential {
    pub api_key: u64,
    pub token: [u8; 32],
    pub role: Role,
    pub enabled: bool,
}

#[derive(Debug, Default)]
pub struct CredentialStore {
    credentials: BTreeMap<u64, Credential>,
}

impl CredentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, credential: Credential) {
        self.credentials.insert(credential.api_key, credential);
    }

    pub fn disable(&mut self, api_key: u64) {
        if let Some(credential) = self.credentials.get_mut(&api_key) {
            credential.enabled = false;
        }
    }

    pub fn authenticate(&self, api_key: u64, token: &[u8; 32]) -> Result<Role, ProtocolError> {
        let credential = self
            .credentials
            .get(&api_key)
            .ok_or(ProtocolError::AuthenticationFailed)?;
        if !credential.enabled || !constant_time_eq(&credential.token, token) {
            return Err(ProtocolError::AuthenticationFailed);
        }
        Ok(credential.role)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireMessage {
    Logon {
        api_key: u64,
        token: [u8; 32],
        heartbeat_ticks: u64,
    },
    Order(OrderCommand),
    ExecutionReport {
        exchange_order_id: u64,
        client_order_id: u64,
        status: u8,
        filled_quantity: u32,
        remaining_quantity: u32,
    },
    Reject {
        code: u16,
    },
    Heartbeat,
    ResendRequest {
        begin: MessageSeq,
        end: MessageSeq,
    },
    Logout {
        reason: u16,
    },
    MarketData {
        instrument_id: u16,
        sequence: u64,
        side: u8,
        price: u32,
        quantity: u32,
    },
    Admin(AdminCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminCommand {
    HaltInstrument(u16),
    ResumeInstrument(u16),
    CancelAllAccount(u32),
    FreezeAccount(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireFrame {
    pub sequence: MessageSeq,
    pub message: WireMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    Truncated,
    BadMagic,
    UnsupportedVersion(u8),
    PayloadTooLarge,
    InvalidLength,
    InvalidPayload,
    ChecksumMismatch,
    UnknownMessageType(u8),
    AuthenticationFailed,
    NotAuthenticated,
    PermissionDenied,
    Session(SessionError),
}

impl From<SessionError> for ProtocolError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl WireFrame {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let payload = encode_message(&self.message)?;
        if payload.len() > MAX_PAYLOAD {
            return Err(ProtocolError::PayloadTooLarge);
        }

        let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(message_type(&self.message) as u8);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&self.sequence.0.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&crc32(&payload).to_le_bytes());
        out.extend_from_slice(&payload);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < HEADER_LEN {
            return Err(ProtocolError::Truncated);
        }
        if &bytes[0..4] != MAGIC {
            return Err(ProtocolError::BadMagic);
        }
        if bytes[4] != VERSION {
            return Err(ProtocolError::UnsupportedVersion(bytes[4]));
        }

        let message_type = MessageType::from_u8(bytes[5])?;
        let sequence = MessageSeq(u64::from_le_bytes(
            bytes[8..16].try_into().map_err(|_| ProtocolError::Truncated)?,
        ));
        let payload_len =
            u32::from_le_bytes(bytes[16..20].try_into().map_err(|_| ProtocolError::Truncated)?)
                as usize;
        let expected_crc =
            u32::from_le_bytes(bytes[20..24].try_into().map_err(|_| ProtocolError::Truncated)?);

        if payload_len > MAX_PAYLOAD {
            return Err(ProtocolError::PayloadTooLarge);
        }
        if bytes.len() != HEADER_LEN + payload_len {
            return Err(ProtocolError::InvalidLength);
        }

        let payload = &bytes[HEADER_LEN..];
        if crc32(payload) != expected_crc {
            return Err(ProtocolError::ChecksumMismatch);
        }

        Ok(Self {
            sequence,
            message: decode_message(message_type, payload)?,
        })
    }
}

#[derive(Debug)]
pub enum GatewayAction {
    Accepted(OrderCommand),
    Send(WireMessage),
    Admin(AdminCommand),
}

#[derive(Debug)]
pub struct GatewaySession {
    session: SessionState,
    role: Option<Role>,
    api_key: Option<u64>,
}

impl GatewaySession {
    pub fn new() -> Self {
        Self {
            session: SessionState::new(),
            role: None,
            api_key: None,
        }
    }

    pub fn role(&self) -> Option<Role> {
        self.role
    }

    pub fn api_key(&self) -> Option<u64> {
        self.api_key
    }

    pub fn receive(
        &mut self,
        frame: WireFrame,
        credentials: &CredentialStore,
    ) -> Result<Vec<GatewayAction>, ProtocolError> {
        if !matches!(frame.message, WireMessage::Logon { .. }) && self.role.is_none() {
            return Err(ProtocolError::NotAuthenticated);
        }

        match frame.message {
            WireMessage::Logon {
                api_key,
                token,
                heartbeat_ticks,
            } => {
                if self.role.is_some() {
                    return Err(ProtocolError::PermissionDenied);
                }
                let role = credentials.authenticate(api_key, &token)?;
                self.session
                    .establish(MessageSeq(frame.sequence.0 + 1), MessageSeq(1), heartbeat_ticks);
                self.role = Some(role);
                self.api_key = Some(api_key);
                Ok(vec![GatewayAction::Send(WireMessage::Heartbeat)])
            }
            WireMessage::Order(command) => {
                self.session.receive(frame.sequence)?;
                let permission = match command {
                    OrderCommand::New(_) => Permission::SubmitOrders,
                    OrderCommand::Cancel(_) => Permission::CancelOrders,
                    OrderCommand::Replace(_) => Permission::ReplaceOrders,
                };
                self.require(permission)?;
                Ok(vec![GatewayAction::Accepted(command)])
            }
            WireMessage::Heartbeat => {
                self.session.receive(frame.sequence)?;
                self.session.note_activity(frame.sequence.0);
                Ok(vec![GatewayAction::Send(WireMessage::Heartbeat)])
            }
            WireMessage::ResendRequest { begin, end } => {
                self.session.receive(frame.sequence)?;
                self.require(Permission::RequestRecovery)?;
                if begin > end {
                    return Err(ProtocolError::InvalidPayload);
                }
                Ok(Vec::new())
            }
            WireMessage::Logout { .. } => {
                self.session.receive(frame.sequence)?;
                Ok(Vec::new())
            }
            WireMessage::Admin(command) => {
                self.session.receive(frame.sequence)?;
                self.require(Permission::OperateMarket)?;
                Ok(vec![GatewayAction::Admin(command)])
            }
            _ => {
                self.session.receive(frame.sequence)?;
                self.require(Permission::ViewMarketData)?;
                Ok(Vec::new())
            }
        }
    }

    pub fn next_outbound(&mut self, message: WireMessage) -> Result<WireFrame, ProtocolError> {
        let sequence = self.session.allocate_outbound()?;
        Ok(WireFrame { sequence, message })
    }

    fn require(&self, permission: Permission) -> Result<(), ProtocolError> {
        if self
            .role
            .is_some_and(|role| role.allows(permission))
        {
            Ok(())
        } else {
            Err(ProtocolError::PermissionDenied)
        }
    }
}

impl Default for GatewaySession {
    fn default() -> Self {
        Self::new()
    }
}

fn message_type(message: &WireMessage) -> MessageType {
    match message {
        WireMessage::Logon { .. } => MessageType::Logon,
        WireMessage::Order(OrderCommand::New(_)) => MessageType::NewOrder,
        WireMessage::Order(OrderCommand::Cancel(_)) => MessageType::CancelOrder,
        WireMessage::Order(OrderCommand::Replace(_)) => MessageType::ReplaceOrder,
        WireMessage::ExecutionReport { .. } => MessageType::ExecutionReport,
        WireMessage::Reject { .. } => MessageType::Reject,
        WireMessage::Heartbeat => MessageType::Heartbeat,
        WireMessage::ResendRequest { .. } => MessageType::ResendRequest,
        WireMessage::Logout { .. } => MessageType::Logout,
        WireMessage::MarketData { .. } => MessageType::MarketData,
        WireMessage::Admin(_) => MessageType::Admin,
    }
}

fn encode_message(message: &WireMessage) -> Result<Vec<u8>, ProtocolError> {
    let mut out = Vec::with_capacity(128);
    match message {
        WireMessage::Logon {
            api_key,
            token,
            heartbeat_ticks,
        } => {
            put_u64(&mut out, *api_key);
            out.extend_from_slice(token);
            put_u64(&mut out, *heartbeat_ticks);
        }
        WireMessage::Order(OrderCommand::New(order)) => {
            put_u64(&mut out, order.client_order_id.0);
            put_u32(&mut out, order.account_id);
            put_u16(&mut out, order.instrument_id);
            out.push(order.side.wire_value());
            put_u32(&mut out, order.price);
            put_u32(&mut out, order.quantity);
            put_u64(&mut out, order.client_timestamp);
        }
        WireMessage::Order(OrderCommand::Cancel(order)) => {
            put_u32(&mut out, order.account_id);
            put_u16(&mut out, order.instrument_id);
            put_u64(&mut out, order.client_order_id.0);
        }
        WireMessage::Order(OrderCommand::Replace(order)) => {
            put_u32(&mut out, order.account_id);
            put_u16(&mut out, order.instrument_id);
            put_u64(&mut out, order.target_client_order_id.0);
            put_u64(&mut out, order.new_client_order_id.0);
            out.push(order.side.wire_value());
            put_u32(&mut out, order.price);
            put_u32(&mut out, order.quantity);
            put_u64(&mut out, order.client_timestamp);
        }
        WireMessage::ExecutionReport {
            exchange_order_id,
            client_order_id,
            status,
            filled_quantity,
            remaining_quantity,
        } => {
            put_u64(&mut out, *exchange_order_id);
            put_u64(&mut out, *client_order_id);
            out.push(*status);
            put_u32(&mut out, *filled_quantity);
            put_u32(&mut out, *remaining_quantity);
        }
        WireMessage::Reject { code } => put_u16(&mut out, *code),
        WireMessage::Heartbeat => {}
        WireMessage::ResendRequest { begin, end } => {
            put_u64(&mut out, begin.0);
            put_u64(&mut out, end.0);
        }
        WireMessage::Logout { reason } => put_u16(&mut out, *reason),
        WireMessage::MarketData {
            instrument_id,
            sequence,
            side,
            price,
            quantity,
        } => {
            put_u16(&mut out, *instrument_id);
            put_u64(&mut out, *sequence);
            out.push(*side);
            put_u32(&mut out, *price);
            put_u32(&mut out, *quantity);
        }
        WireMessage::Admin(command) => match command {
            AdminCommand::HaltInstrument(id) => {
                out.push(1);
                put_u16(&mut out, *id);
            }
            AdminCommand::ResumeInstrument(id) => {
                out.push(2);
                put_u16(&mut out, *id);
            }
            AdminCommand::CancelAllAccount(id) => {
                out.push(3);
                put_u32(&mut out, *id);
            }
            AdminCommand::FreezeAccount(id) => {
                out.push(4);
                put_u32(&mut out, *id);
            }
        },
    }
    Ok(out)
}

fn decode_message(message_type: MessageType, bytes: &[u8]) -> Result<WireMessage, ProtocolError> {
    let mut cursor = Cursor::new(bytes);
    match message_type {
        MessageType::Logon => Ok(WireMessage::Logon {
            api_key: cursor.u64()?,
            token: cursor.array32()?,
            heartbeat_ticks: cursor.u64()?,
        }),
        MessageType::NewOrder => {
            let side = decode_side(cursor.u8()?)?;
            Ok(WireMessage::Order(OrderCommand::New(NewOrder {
                client_order_id: ClientOrderId(cursor.u64_at(0)?),
                account_id: cursor.u32_at(8)?,
                instrument_id: cursor.u16_at(12)?,
                side,
                price: cursor.u32_at(13)?,
                quantity: cursor.u32_at(17)?,
                client_timestamp: cursor.u64_at(21)?,
            })))
        }
        MessageType::CancelOrder => Ok(WireMessage::Order(OrderCommand::Cancel(CancelOrder {
            account_id: cursor.u32()?,
            instrument_id: cursor.u16()?,
            client_order_id: ClientOrderId(cursor.u64()?),
        }))),
        MessageType::ReplaceOrder => {
            let account_id = cursor.u32()?;
            let instrument_id = cursor.u16()?;
            let target = ClientOrderId(cursor.u64()?);
            let new_id = ClientOrderId(cursor.u64()?);
            let side = decode_side(cursor.u8()?)?;
            let price = cursor.u32()?;
            let quantity = cursor.u32()?;
            let timestamp = cursor.u64()?;
            Ok(WireMessage::Order(OrderCommand::Replace(ReplaceOrder {
                account_id,
                instrument_id,
                target_client_order_id: target,
                new_client_order_id: new_id,
                side,
                price,
                quantity,
                client_timestamp: timestamp,
            })))
        }
        MessageType::ExecutionReport => Ok(WireMessage::ExecutionReport {
            exchange_order_id: cursor.u64()?,
            client_order_id: cursor.u64()?,
            status: cursor.u8()?,
            filled_quantity: cursor.u32()?,
            remaining_quantity: cursor.u32()?,
        }),
        MessageType::Reject => Ok(WireMessage::Reject {
            code: cursor.u16()?,
        }),
        MessageType::Heartbeat => Ok(WireMessage::Heartbeat),
        MessageType::ResendRequest => Ok(WireMessage::ResendRequest {
            begin: MessageSeq(cursor.u64()?),
            end: MessageSeq(cursor.u64()?),
        }),
        MessageType::Logout => Ok(WireMessage::Logout {
            reason: cursor.u16()?,
        }),
        MessageType::MarketData => Ok(WireMessage::MarketData {
            instrument_id: cursor.u16()?,
            sequence: cursor.u64()?,
            side: cursor.u8()?,
            price: cursor.u32()?,
            quantity: cursor.u32()?,
        }),
        MessageType::Admin => {
            let code = cursor.u8()?;
            let command = match code {
                1 => AdminCommand::HaltInstrument(cursor.u16()?),
                2 => AdminCommand::ResumeInstrument(cursor.u16()?),
                3 => AdminCommand::CancelAllAccount(cursor.u32()?),
                4 => AdminCommand::FreezeAccount(cursor.u32()?),
                _ => return Err(ProtocolError::InvalidPayload),
            };
            Ok(WireMessage::Admin(command))
        }
    }
}

fn decode_side(value: u8) -> Result<OrderSide, ProtocolError> {
    match value {
        0 => Ok(OrderSide::Buy),
        1 => Ok(OrderSide::Sell),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(ProtocolError::InvalidPayload)?;
        if end > self.bytes.len() {
            return Err(ProtocolError::Truncated);
        }
        let value = self.bytes[self.offset..end]
            .try_into()
            .map_err(|_| ProtocolError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_le_bytes(self.take::<2>()?))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_le_bytes(self.take::<4>()?))
    }

    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_le_bytes(self.take::<8>()?))
    }

    fn array32(&mut self) -> Result<[u8; 32], ProtocolError> {
        self.take::<32>()
    }

    fn u64_at(&self, offset: usize) -> Result<u64, ProtocolError> {
        self.bytes
            .get(offset..offset + 8)
            .ok_or(ProtocolError::Truncated)
            .and_then(|slice| {
                slice
                    .try_into()
                    .map(u64::from_le_bytes)
                    .map_err(|_| ProtocolError::Truncated)
            })
    }

    fn u32_at(&self, offset: usize) -> Result<u32, ProtocolError> {
        self.bytes
            .get(offset..offset + 4)
            .ok_or(ProtocolError::Truncated)
            .and_then(|slice| {
                slice
                    .try_into()
                    .map(u32::from_le_bytes)
                    .map_err(|_| ProtocolError::Truncated)
            })
    }

    fn u16_at(&self, offset: usize) -> Result<u16, ProtocolError> {
        self.bytes
            .get(offset..offset + 2)
            .ok_or(ProtocolError::Truncated)
            .and_then(|slice| {
                slice
                    .try_into()
                    .map(u16::from_le_bytes)
                    .map_err(|_| ProtocolError::Truncated)
            })
    }
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= left[i] ^ right[i];
    }
    diff == 0
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credential() -> Credential {
        Credential {
            api_key: 7,
            token: [9; 32],
            role: Role::Trader,
            enabled: true,
        }
    }

    #[test]
    fn frame_round_trip_preserves_order() {
        let frame = WireFrame {
            sequence: MessageSeq(1),
            message: WireMessage::Order(OrderCommand::New(NewOrder {
                client_order_id: ClientOrderId(11),
                account_id: 4,
                instrument_id: 2,
                side: OrderSide::Buy,
                price: 100,
                quantity: 5,
                client_timestamp: 99,
            })),
        };
        let bytes = frame.encode().unwrap();
        assert_eq!(WireFrame::decode(&bytes).unwrap(), frame);
    }

    #[test]
    fn corrupt_frame_fails_closed() {
        let frame = WireFrame {
            sequence: MessageSeq(1),
            message: WireMessage::Heartbeat,
        };
        let mut bytes = frame.encode().unwrap();
        bytes[23] ^= 1;
        assert_eq!(
            WireFrame::decode(&bytes),
            Err(ProtocolError::ChecksumMismatch)
        );
    }

    #[test]
    fn gateway_requires_authentication_and_sequence_order() {
        let mut credentials = CredentialStore::new();
        credentials.register(credential());
        let mut session = GatewaySession::new();

        let rejected = session.receive(
            WireFrame {
                sequence: MessageSeq(1),
                message: WireMessage::Order(OrderCommand::New(NewOrder {
                    client_order_id: ClientOrderId(1),
                    account_id: 4,
                    instrument_id: 0,
                    side: OrderSide::Buy,
                    price: 100,
                    quantity: 1,
                    client_timestamp: 1,
                })),
            },
            &credentials,
        );
        assert_eq!(rejected, Err(ProtocolError::NotAuthenticated));

        session
            .receive(
                WireFrame {
                    sequence: MessageSeq(1),
                    message: WireMessage::Logon {
                        api_key: 7,
                        token: [9; 32],
                        heartbeat_ticks: 100,
                    },
                },
                &credentials,
            )
            .unwrap();

        let actions = session
            .receive(
                WireFrame {
                    sequence: MessageSeq(2),
                    message: WireMessage::Order(OrderCommand::New(NewOrder {
                        client_order_id: ClientOrderId(2),
                        account_id: 4,
                        instrument_id: 0,
                        side: OrderSide::Buy,
                        price: 100,
                        quantity: 1,
                        client_timestamp: 2,
                    })),
                },
                &credentials,
            )
            .unwrap();
        assert!(matches!(actions[0], GatewayAction::Accepted(_)));

        assert_eq!(
            session
                .receive(
                    WireFrame {
                        sequence: MessageSeq(4),
                        message: WireMessage::Heartbeat,
                    },
                    &credentials,
                )
                .unwrap_err(),
            ProtocolError::Session(SessionError::Gap {
                expected: MessageSeq(3),
                received: MessageSeq(4),
            })
        );
    }

    #[test]
    fn read_only_cannot_submit_orders() {
        let mut credentials = CredentialStore::new();
        credentials.register(Credential {
            api_key: 8,
            token: [3; 32],
            role: Role::ReadOnly,
            enabled: true,
        });
        let mut session = GatewaySession::new();
        session
            .receive(
                WireFrame {
                    sequence: MessageSeq(1),
                    message: WireMessage::Logon {
                        api_key: 8,
                        token: [3; 32],
                        heartbeat_ticks: 10,
                    },
                },
                &credentials,
            )
            .unwrap();
        assert_eq!(
            session
                .receive(
                    WireFrame {
                        sequence: MessageSeq(2),
                        message: WireMessage::Order(OrderCommand::New(NewOrder {
                            client_order_id: ClientOrderId(1),
                            account_id: 1,
                            instrument_id: 0,
                            side: OrderSide::Buy,
                            price: 1,
                            quantity: 1,
                            client_timestamp: 1,
                        })),
                    },
                    &credentials,
                )
                .unwrap_err(),
            ProtocolError::PermissionDenied
        );
    }
}
