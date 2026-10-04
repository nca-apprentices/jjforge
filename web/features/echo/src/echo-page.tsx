import { echo } from "@jjforge/api";
import { Button, Form, Output, Page, TextField } from "@jjforge/ui";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";

/**
 * Sends a message through the server and vcs and shows what came back.
 * @public
 */
export function EchoPage() {
  const send = useMutation({ mutationFn: echo });
  const form = useForm({
    defaultValues: { message: "" },
    onSubmit: ({ value }) => send.mutate(value.message),
  });

  return (
    <Page title="jjforge">
      <Form onSubmit={() => void form.handleSubmit()}>
        <form.Field name="message">
          {(field) => (
            <TextField
              label="Message"
              value={field.state.value}
              onChange={field.handleChange}
            />
          )}
        </form.Field>
        <Button type="submit">Echo</Button>
      </Form>
      <Output>{send.isError ? String(send.error) : send.data}</Output>
    </Page>
  );
}
