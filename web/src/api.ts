/** Calls `POST /api/echo`, the one operation in /openapi.yaml. */
export async function echo(message: string): Promise<string> {
  const response = await fetch("/api/echo", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ message }),
  });

  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText}`);
  }

  const body = (await response.json()) as { message: string };
  return body.message;
}
