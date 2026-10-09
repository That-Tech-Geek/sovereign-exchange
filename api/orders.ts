import { proxyToEngine } from "./_lib/engine";

export async function POST(request: Request): Promise<Response> {
  return proxyToEngine("/orders", request, ["POST"]);
}
