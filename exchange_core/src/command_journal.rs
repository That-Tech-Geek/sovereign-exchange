use crate::command::{CancelOrder, NewOrder, OrderCommand, OrderSide, ReplaceOrder};
use crate::order::ClientOrderId;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"SEC1";
const VERSION: u16 = 1;
const HEADER: usize = 16;
const MAX_RECORD: usize = 128;

#[derive(Debug)]
pub enum CommandJournalError {
    Io(io::Error),
    Invalid(&'static str),
    Checksum,
    UnsupportedVersion(u16),
}
impl From<io::Error> for CommandJournalError { fn from(e: io::Error) -> Self { Self::Io(e) } }
impl std::fmt::Display for CommandJournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "command journal I/O error: {e}"),
            Self::Invalid(m) => write!(f, "invalid command journal: {m}"),
            Self::Checksum => write!(f, "command journal checksum mismatch"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported command journal version {v}"),
        }
    }
}
impl std::error::Error for CommandJournalError {}

pub struct CommandJournal { file: File, path: PathBuf }
impl CommandJournal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CommandJournalError> {
        let path=path.as_ref().to_path_buf();
        let file=OpenOptions::new().create(true).read(true).append(true).open(&path)?;
        let mut j=Self{file,path};
        j.validate()?;
        Ok(j)
    }
    pub fn path(&self)->&Path { &self.path }
    pub fn append(&mut self, command:&OrderCommand)->Result<(),CommandJournalError> {
        let payload=encode(command);
        if payload.len()>MAX_RECORD { return Err(CommandJournalError::Invalid("record too large")); }
        let checksum=crc32(&payload);
        let mut h=[0u8;HEADER];
        h[0..4].copy_from_slice(MAGIC);
        h[4..6].copy_from_slice(&VERSION.to_le_bytes());
        h[6]=payload[0];
        h[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        h[12..16].copy_from_slice(&checksum.to_le_bytes());
        self.file.write_all(&h)?;
        self.file.write_all(&payload)?;
        self.file.sync_data()?;
        Ok(())
    }
    pub fn read_all(&mut self)->Result<Vec<OrderCommand>,CommandJournalError>{
        self.file.rewind()?;
        let mut bytes=Vec::new(); self.file.read_to_end(&mut bytes)?;
        decode_records(&bytes)
    }
    fn validate(&mut self)->Result<(),CommandJournalError>{ let _=self.read_all()?; self.file.seek(std::io::SeekFrom::End(0))?; Ok(()) }
}

fn encode(c:&OrderCommand)->Vec<u8>{
    let mut o=Vec::with_capacity(64);
    match *c {
        OrderCommand::New(x)=>{o.push(1); put_u64(&mut o,x.client_order_id.0); put_u32(&mut o,x.account_id); put_u16(&mut o,x.instrument_id); o.push(x.side.wire_value()); put_u32(&mut o,x.price); put_u32(&mut o,x.quantity); put_u64(&mut o,x.client_timestamp);}
        OrderCommand::Cancel(x)=>{o.push(2); put_u32(&mut o,x.account_id); put_u16(&mut o,x.instrument_id); put_u64(&mut o,x.client_order_id.0);}
        OrderCommand::Replace(x)=>{o.push(3); put_u32(&mut o,x.account_id); put_u16(&mut o,x.instrument_id); put_u64(&mut o,x.target_client_order_id.0); put_u64(&mut o,x.new_client_order_id.0); o.push(x.side.wire_value()); put_u32(&mut o,x.price); put_u32(&mut o,x.quantity); put_u64(&mut o,x.client_timestamp);}
    } o
}
fn decode_records(bytes:&[u8])->Result<Vec<OrderCommand>,CommandJournalError>{
    let mut out=Vec::new(); let mut p=0;
    while p<bytes.len(){
        if bytes.len()-p<HEADER{return Err(CommandJournalError::Invalid("truncated header"))}
        if &bytes[p..p+4]!=MAGIC{return Err(CommandJournalError::Invalid("bad magic"))}
        let v=u16::from_le_bytes([bytes[p+4],bytes[p+5]]); if v!=VERSION{return Err(CommandJournalError::UnsupportedVersion(v))}
        let n=u32::from_le_bytes(bytes[p+8..p+12].try_into().unwrap()) as usize;
        if n==0||n>MAX_RECORD||p.checked_add(HEADER+n).filter(|x|*x<=bytes.len()).is_none(){return Err(CommandJournalError::Invalid("invalid record length"))}
        let end=p+HEADER+n; let payload=&bytes[p+HEADER..end];
        if crc32(payload)!=u32::from_le_bytes(bytes[p+12..p+16].try_into().unwrap()){return Err(CommandJournalError::Checksum)}
        out.push(decode(payload)?); p=end;
    } Ok(out)
}
fn decode(p:&[u8])->Result<OrderCommand,CommandJournalError>{
    let mut r=Reader{b:p,p:0}; let kind=r.u8()?;
    let side=|v|match v{0=>Ok(OrderSide::Buy),1=>Ok(OrderSide::Sell),_=>
        Err(CommandJournalError::Invalid("invalid side"))};
    match kind{
        1=>Ok(OrderCommand::New(NewOrder{client_order_id:ClientOrderId(r.u64()?),account_id:r.u32()?,instrument_id:r.u16()?,side:side(r.u8()?)?,price:r.u32()?,quantity:r.u32()?,client_timestamp:r.u64()?})),
        2=>Ok(OrderCommand::Cancel(CancelOrder{account_id:r.u32()?,instrument_id:r.u16()?,client_order_id:ClientOrderId(r.u64()?)})),
        3=>Ok(OrderCommand::Replace(ReplaceOrder{account_id:r.u32()?,instrument_id:r.u16()?,target_client_order_id:ClientOrderId(r.u64()?),new_client_order_id:ClientOrderId(r.u64()?),side:side(r.u8()?)?,price:r.u32()?,quantity:r.u32()?,client_timestamp:r.u64()?})),
        _=>Err(CommandJournalError::Invalid("unknown command type"))
    }
}
struct Reader<'a>{b:&'a[u8],p:usize}
impl<'a> Reader<'a>{
 fn take(&mut self,n:usize)->Result<&'a[u8],CommandJournalError>{let e=self.p.checked_add(n).ok_or(CommandJournalError::Invalid("offset overflow"))?;if e>self.b.len(){return Err(CommandJournalError::Invalid("truncated payload"))}let x=&self.b[self.p..e];self.p=e;Ok(x)}
 fn u8(&mut self)->Result<u8,CommandJournalError>{Ok(self.take(1)?[0])}
 fn u16(&mut self)->Result<u16,CommandJournalError>{Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))}
 fn u32(&mut self)->Result<u32,CommandJournalError>{Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))}
 fn u64(&mut self)->Result<u64,CommandJournalError>{Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))}
}
fn put_u16(o:&mut Vec<u8>,v:u16){o.extend_from_slice(&v.to_le_bytes())}
fn put_u32(o:&mut Vec<u8>,v:u32){o.extend_from_slice(&v.to_le_bytes())}
fn put_u64(o:&mut Vec<u8>,v:u64){o.extend_from_slice(&v.to_le_bytes())}
fn crc32(b:&[u8])->u32{let mut c=0xffff_ffffu32;for &x in b{c^=x as u32;for _ in 0..8{c=if c&1!=0{(c>>1)^0xedb8_8320}else{c>>1}}}!c}
