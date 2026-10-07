import { forwardRef, type ComponentPropsWithoutRef, type CSSProperties, type ReactNode, type MouseEventHandler } from "react";
import { CloseButton } from "./CloseButton";

/** Keep the glass frame outside the scrolling content viewport. */
export const ModalSurface = forwardRef<HTMLDivElement, ComponentPropsWithoutRef<"div"> & { onDismiss?: () => void; closeDisabled?: boolean; hideClose?: boolean; header?: ReactNode; headerActions?: ReactNode; onHeaderMouseDown?: MouseEventHandler<HTMLDivElement> }>(
  function ModalSurface({ children, className = "", style, onDismiss, closeDisabled, hideClose, header, headerActions, onHeaderMouseDown, ...props }, ref) {
    // Embedded updater content does not create another dialog surface.
    if (!className.split(" ").includes("modal-content")) {
      return <div {...props} ref={ref} className={className} style={style}>{header}{children}</div>;
    }
    const frameStyle = { ...style, "--modal-content-padding": style?.padding ?? "2rem" } as CSSProperties;
    return <div {...props} ref={ref} className={`${className} fixed-glass-frame ${hideClose ? "" : "has-surface-close"} ${header ? "has-dialog-titlebar" : ""}`} style={frameStyle}>
      {header ? <div className="dialog-titlebar" onMouseDown={onHeaderMouseDown}>{header}{headerActions}{!hideClose && <CloseButton onClose={onDismiss} disabled={closeDisabled} />}</div>
        : !hideClose && <CloseButton onClose={onDismiss} disabled={closeDisabled} />}
      <div className="modal-scroll-body">{children}</div>
    </div>;
  },
);
