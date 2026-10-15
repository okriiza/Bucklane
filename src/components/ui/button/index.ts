import { cva, type VariantProps } from "class-variance-authority";

export { default as Button } from "./Button.vue";

export const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 rounded-md text-[13px] font-medium whitespace-nowrap outline-none transition-[background-color,color,box-shadow,transform] duration-150 select-none active:scale-[0.97] focus-visible:ring-[3px] focus-visible:ring-ring/40 disabled:pointer-events-none disabled:opacity-45 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground shadow-[var(--shadow-soft)] hover:bg-primary/90",
        destructive:
          "bg-destructive text-white shadow-[var(--shadow-soft)] hover:bg-destructive/90",
        outline:
          "border bg-background shadow-[var(--shadow-soft)] hover:bg-accent hover:text-accent-foreground dark:bg-input/30",
        secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/70",
        ghost: "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
      },
      size: {
        default: "h-8 px-3",
        sm: "h-7 gap-1 px-2.5 text-xs [&_svg]:size-3.5",
        lg: "h-10 px-5 text-sm",
        icon: "size-8",
        "icon-sm": "size-7 [&_svg]:size-3.5",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export type ButtonVariants = VariantProps<typeof buttonVariants>;
