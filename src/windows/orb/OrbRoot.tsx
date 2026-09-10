import { getCurrentWindow } from "@tauri-apps/api/window";
import { commands } from "../../ipc/bindings";

export function OrbRoot() {
  const beginDrag = () => {
    void commands.orbBeginDrag().catch(() => undefined);
  };

  const saveDroppedPosition = () => {
    void getCurrentWindow()
      .outerPosition()
      .then((position) => commands.orbDropped(position.x, position.y))
      .catch(() => undefined);
  };

  return (
    <div
      aria-label="Trident orb"
      className="orb"
      role="button"
      onPointerDown={beginDrag}
      onPointerUp={saveDroppedPosition}
    >
      <span aria-hidden="true" className="orb__dot" />
      <span>Trident</span>
    </div>
  );
}
