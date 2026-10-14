import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
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
});

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
