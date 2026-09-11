export interface CountryTickerInfo {
  id: number;
  symbol: string;
  name: string;
  currency: string;
  flag: string;
  tickSize: number;
  basePrice: number; // e.g. 34500 ($345.00)
  totalShares: number;
}

export interface ClobLevel {
  price: number;
  priceFormatted: string;
  volume: number;
  orderCount: number;
  cumulativeVolume: number;
  percentage: number;
}

export interface TradeRecord {
  id: string;
  tickerId: number;
  tickerSymbol: string;
  price: number;
  qty: number;
  buyer: number;
  seller: number;
  timestampNanos: number;
  timestampFormatted: string;
}

export interface SimulatedOrder {
  orderId: number;
  accountId: number;
  tickerId: number;
  side: 0 | 1; // 0 = Buy, 1 = Sell
  price: number;
  quantity: number;
  remaining: number;
  timestamp: number;
}

export interface PerformanceStats {
  ordersIngested: number;
  tradesExecuted: number;
  currentServiceTimeMicros: number;
  p50LatencyMicros: number;
  p99LatencyMicros: number;
  p999LatencyMicros: number;
  ordersPerSec: number;
  core0Load: number; // %
  core1Load: number; // %
  activeAllocatedOrders: number;
  freeHeadIndex: number;
  ringBufferDepth: number;
  firestoreFlushes: number;
  walBytesBuffered: number;
  snapshotsTaken: number;
  activeBotsCount: number;
  swarmStatus: 'IDLE' | 'ACTIVE_10K';
}

export interface RustFileDoc {
  path: string;
  name: string;
  description: string;
  code: string;
}
