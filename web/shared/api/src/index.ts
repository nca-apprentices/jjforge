import createClient from "openapi-fetch";
import type { paths } from "./contract.gen";

/**
 * The typed client for /shared/openapi.yaml, generated into contract.gen.ts.
 * It is the only code in web/ that sends a request.
 * @public
 */
export const client = createClient<paths>({ baseUrl: location.origin });

/**
 * Calls `POST /api/echo` and returns the message vcsd answered.
 * @public
 */
export async function echo(message: string): Promise<string> {
  const { data, response } = await client.POST("/api/echo", {
    body: { message },
  });

  if (data === undefined) {
    throw new Error(`${response.status} ${response.statusText}`);
  }

  return data.message;
}
