import { fireEvent, render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  orbBeginDrag: vi.fn(),
  orbDropped: vi.fn(),
  outerPosition: vi.fn(),
}));

vi.mock("../../ipc/bindings", () => ({
  commands: {
    orbBeginDrag: mocks.orbBeginDrag,
    orbDropped: mocks.orbDropped,
  },
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ outerPosition: mocks.outerPosition }),
}));

import { OrbRoot } from "./OrbRoot";

describe("OrbRoot", () => {
  it("contains no focusable controls", () => {
    const { container } = render(<OrbRoot />);

    expect(container.querySelectorAll("button, input, [tabindex]")).toHaveLength(0);
  });

  it("uses physical window coordinates when the drag ends", async () => {
    mocks.orbBeginDrag.mockResolvedValue({ status: "ok", data: null });
    mocks.outerPosition.mockResolvedValue({ x: 320, y: 240 });
    mocks.orbDropped.mockResolvedValue({ status: "ok", data: {} });
    const { container } = render(<OrbRoot />);
    const orb = container.querySelector(".orb");

    if (orb === null) {
      throw new Error("the orb root must render");
    }

    fireEvent.pointerDown(orb);
    fireEvent.pointerUp(orb);

    await waitFor(() => {
      expect(mocks.orbBeginDrag).toHaveBeenCalledOnce();
      expect(mocks.orbDropped).toHaveBeenCalledWith(320, 240);
    });
  });
});
