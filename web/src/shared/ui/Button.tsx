import type { ButtonHTMLAttributes } from "react";

import { buttonClassName, type ButtonSize, type ButtonVariant } from "./button-class-name";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

/** Restrained macOS-style command with a stable fill for every text button. */
export function Button({
  className,
  type = "button",
  variant = "secondary",
  size = "md",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      data-variant={variant}
      className={buttonClassName({ variant, size, className })}
      {...props}
    />
  );
}
