import { type FormEvent, useState } from "react";
import { echo } from "./api";

/** Sends a message through the server and vcsd and shows what came back. */
export function EchoForm() {
  const [message, setMessage] = useState("");
  const [answer, setAnswer] = useState("");

  async function submit(event: FormEvent) {
    event.preventDefault();
    try {
      setAnswer(await echo(message));
    } catch (error) {
      setAnswer(String(error));
    }
  }

  return (
    <main>
      <h1>jjforge</h1>
      <form onSubmit={submit}>
        <input
          aria-label="Message"
          value={message}
          onChange={(event) => setMessage(event.target.value)}
        />
        <button type="submit">Echo</button>
      </form>
      <output>{answer}</output>
    </main>
  );
}
