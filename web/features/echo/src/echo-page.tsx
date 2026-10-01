import { echo } from "@jjforge/api";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";

/**
 * Sends a message through the server and vcsd and shows what came back.
 * @public
 */
export function EchoPage() {
  const send = useMutation({ mutationFn: echo });
  const form = useForm({
    defaultValues: { message: "" },
    onSubmit: ({ value }) => send.mutate(value.message),
  });

  return (
    <main>
      <h1>jjforge</h1>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <form.Field name="message">
          {(field) => (
            <input
              aria-label="Message"
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
            />
          )}
        </form.Field>
        <button type="submit">Echo</button>
      </form>
      <output>{send.isError ? String(send.error) : send.data}</output>
    </main>
  );
}
