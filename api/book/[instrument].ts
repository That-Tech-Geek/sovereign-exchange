import { proxyToEngine } from "../_lib/engine";

export async function GET(request: Request): Promise<Response> {
  const instrument = new URL(request.url).pathname.split("/").pop();
  if (!instrument || !/^\d{1,3}$/.test(instrument)) {
    return Response.json({ error: "invalid_instrument" }, { status: 400 });
  }
  return proxyToEngine(`/book/${instrument}`, request, ["GET"]);
}
