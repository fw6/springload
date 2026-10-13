import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Tooltip } from "./Tooltip";
import { ConfidencePill } from "./ConfidencePill";
import { Flag } from "./Flag";

describe("Tooltip", () => {
  it("渲染触发文本与提示内容", () => {
    render(
      <Tooltip text="近 1 个月的涨幅排名落在前 20%~30% 这一段">
        <span>排名</span>
      </Tooltip>,
    );
    expect(screen.getByText("排名")).toBeTruthy();
    const body = screen.getByRole("tooltip");
    expect(body.textContent).toBe("近 1 个月的涨幅排名落在前 20%~30% 这一段");
  });
});

describe("ConfidencePill", () => {
  it("按档位渲染置信度", () => {
    render(<ConfidencePill level="高" />);
    const pill = screen.getByText("高置信");
    expect(pill.getAttribute("data-level")).toBe("高");
  });
});

describe("Flag", () => {
  it("满足显示对勾，不满足显示破折号", () => {
    const { rerender } = render(<Flag ok />);
    expect(screen.getByText("✓")).toBeTruthy();
    rerender(<Flag ok={false} />);
    expect(screen.getByText("—")).toBeTruthy();
  });
});
