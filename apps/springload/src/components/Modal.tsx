import type { ReactNode } from "react";

/** 通用弹窗：点遮罩或「知道了」关闭。 */
export function Modal({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}
      >
        <h3>{title}</h3>
        {children}
        <button type="button" className="primary modal-close" onClick={onClose}>
          知道了
        </button>
      </div>
    </div>
  );
}
