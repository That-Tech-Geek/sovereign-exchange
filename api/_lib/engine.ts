const ENGINE_URL = process.env.EXCHANGE_ENGINE_URL?.replace(/\/$/, "");

export class EngineConfigurationError extends Error {}

function engineUrl(path: string): string {
  if (!ENGINE_URL) {
    throw new EngineConfigurationError(
      "EXCHANGE_ENGINE_URL is not configured; Vercel is not an authoritative matcher."
    );
  }
  return `${ENGINE_URL}${path}`;
}

export async function proxyToEngine(
  path: string,
  request: Request,
  methods: string[],
): Promise<Response> {
  if (!methods.includes(request.method)) {
    return Response.json(
      { error: "method_not_allowed", allowed: methods },
      { status: 405, headers: { Allow: methods.join(", ") } },
    );
  }

  let url: string;
  try {
    const incoming = new URL(request.url);
    const target = new URL(engineUrl(path));
    target.search = incoming.search;
    url = target.toString();
  } catch (error) {
    if (error instanceof EngineConfigurationError) {
      return Response.json(
        { error: "engine_not_configured", authoritative: false, message: error.message },
        { status: 503 },
      );
    }
    return Response.json({ error: "invalid_engine_configuration" }, { status: 500 });
  }

  const body =
    request.method === "GET" || request.method === "HEAD"
      ? undefined
      : await request.arrayBuffer();

  const headers = new Headers();
  const contentType = request.headers.get("content-type");
  if (contentType) headers.set("content-type", contentType);
  headers.set("accept", "application/json");

  try {
    const upstream = await fetch(url, {
      method: request.method,
      headers,
      body,
      cache: "no-store",
      signal: AbortSignal.timeout(5_000),
    });
    const responseHeaders = new Headers(upstream.headers);
    responseHeaders.set("cache-control", "no-store");
    responseHeaders.set("x-sovereign-exchange-authority", "native-engine");
    return new Response(upstream.body, {
      status: upstream.status,
      headers: responseHeaders,
    });
  } catch {
    return Response.json(
      {
        error: "engine_unreachable",
        authoritative: false,
        message: "The authoritative exchange engine could not be reached.",
      },
      { status: 503 },
    );
  }
}
