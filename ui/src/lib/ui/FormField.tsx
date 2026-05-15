import { Show, type JSX } from "solid-js";

interface FormFieldProps {
  label: string;
  htmlFor: string;
  sublabel?: string;
  error?: string;
  children: JSX.Element;
}

export function FormField(props: FormFieldProps) {
  return (
    <div class="form-field">
      <label class="form-field__label" for={props.htmlFor}>
        {props.label}
      </label>
      {props.children}
      <Show when={props.sublabel}>
        <p class="form-field__sublabel">{props.sublabel}</p>
      </Show>
      <Show when={props.error}>
        <p class="form-field__error" role="alert">
          {props.error}
        </p>
      </Show>
    </div>
  );
}
