import createClient from "openapi-fetch";
import type { paths } from "./contract.gen";

/**
 * The typed client for /shared/openapi.yaml, generated into contract.gen.ts.
 * It is the only code in web/ that sends a request.
 * @public
 */
export const client = createClient<paths>({ baseUrl: location.origin });

/**
 * The trace of a request, from the `traceresponse` header the server sets on
 * every response, so a person can quote it, as ADR 0005 decides.
 */
function traceOf(response: Response): string | undefined {
  return response.headers.get("traceresponse")?.split("-")[1];
}

/**
 * Calls `POST /api/v1/echo` and returns what vcs answered, with the trace of
 * the request. A failure names the trace too.
 * @public
 */
export async function echo(
  message: string,
): Promise<{ message: string; traceId?: string }> {
  const { data, response } = await client.POST("/api/v1/echo", {
    body: { message },
  });
  const traceId = traceOf(response);

  if (data === undefined) {
    throw new Error(
      `${response.status} ${response.statusText}, trace ${traceId ?? "unknown"}`,
    );
  }

  return { message: data.message, traceId };
}
