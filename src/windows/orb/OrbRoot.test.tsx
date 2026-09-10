import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { OrbRoot } from "./OrbRoot";

describe("OrbRoot", () => {
  it("contains no focusable controls", () => {
    const { container } = render(<OrbRoot />);

    expect(container.querySelectorAll("button, input, [tabindex]")).toHaveLength(0);
  });
});
