import assert from "node:assert/strict";
import test from "node:test";
import { permissionPromptLayout } from "./PermissionPrompt.js";

test("permission options and input fit short terminals and retain the selected choice", () => {
  for (const height of [4, 5, 7, 10, 12, 24, 40]) {
    for (const count of [1, 4, 7, 20, 64]) {
      for (const hasInput of [false, true]) {
        for (let selected = 0; selected < count; selected++) {
          const layout = permissionPromptLayout(
            height,
            count,
            hasInput,
            selected,
          );
          const rows =
            4 +
            layout.optionRows +
            Number(layout.showOptionIndicator) +
            (layout.payloadLineBudget > 0 ? 1 + layout.payloadLineBudget : 0) +
            (layout.showFooter ? 2 : 0);
          assert.ok(rows <= height, `${height} rows with ${count} choices`);
          if (layout.optionRows > 0) {
            assert.ok(selected >= layout.optionStart);
            assert.ok(selected < layout.optionStart + layout.optionRows);
          }
        }
      }
    }
  }
});
