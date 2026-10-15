import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Tooltip } from "./Tooltip";
import { ConfidencePill } from "./ConfidencePill";
import { Flag } from "./Flag";

describe("Tooltip", () => {
  it("鼠标停上去显示提示内容", async () => {
    render(
      <Tooltip text="近 1 个月的涨幅排名落在前 20%~30% 这一段">
        <span>排名</span>
      </Tooltip>,
    );
    expect(screen.getByText("排名")).toBeTruthy();

    fireEvent.pointerEnter(screen.getByText("排名"), { pointerType: "mouse", buttons: 0 });

    const body = await screen.findByRole("tooltip");
    expect(body.textContent).toBe("近 1 个月的涨幅排名落在前 20%~30% 这一段");
  });

  it("收起时先放完退场动画再摘掉", async () => {
    render(
      <Tooltip text="提示内容">
        <span>排名</span>
      </Tooltip>,
    );
    const trigger = screen.getByText("排名");
    fireEvent.pointerEnter(trigger, { pointerType: "mouse", buttons: 0 });
    const open = await screen.findByRole("tooltip");
    expect(open.getAttribute("data-state")).toBe("open");
    expect(open.classList.contains("tooltip-in")).toBe(true);

    // 按下即收起；指针移开会先走容错通道，不一定立刻关。
    fireEvent.pointerDown(trigger, { pointerType: "mouse", buttons: 1 });
    // 收起中的那一份还在，但已经 inert 加 aria-hidden，按角色查不到。
    expect(surface()?.getAttribute("data-state")).toBe("closed");
    expect(surface()?.classList.contains("tooltip-out")).toBe(true);

    await waitFor(() => {
      expect(surface()).toBeNull();
    });
  });
});

function surface() {
  return document.querySelector('span[role="tooltip"]');
}

describe("ConfidencePill", () => {
  it("按档位渲染置信度", () => {
    render(<ConfidencePill level="高" />);
    const pill = screen.getByText("高置信").closest("[data-level]");
    expect(pill?.getAttribute("data-level")).toBe("高");
  });
});

describe("Flag", () => {
  it("满足画对勾，不满足画一道横线", () => {
    const { container, rerender } = render(<Flag ok />);
    expect(container.querySelector("svg.lucide-check")).not.toBeNull();
    expect(container.querySelector("svg.lucide-minus")).toBeNull();

    rerender(<Flag ok={false} />);
    expect(container.querySelector("svg.lucide-minus")).not.toBeNull();
    expect(container.querySelector("svg.lucide-check")).toBeNull();
  });
});
