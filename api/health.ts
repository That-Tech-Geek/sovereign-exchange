import { proxyToEngine } from "./_lib/engine";

export async function GET(request: Request): Promise<Response> {
  return proxyToEngine("/health", request, ["GET"]);
}
