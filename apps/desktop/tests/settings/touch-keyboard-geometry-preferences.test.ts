import { expect, test } from "vitest";
import { defaultTouchKeyboardGeometry } from "../../../../packages/ui/src/settings/touch-keyboard-geometry-defaults";
import { touchKeyboardGeometryPreferences } from "../../../../packages/ui/src/settings/touch-keyboard-geometry-preferences";

test("uses shared defaults for missing touch keyboard geometry fields", () => {
  expect(touchKeyboardGeometryPreferences()).toEqual(defaultTouchKeyboardGeometry);
  expect(touchKeyboardGeometryPreferences({ touch_keyboard_height_adjustment: 0 })).toEqual(
    defaultTouchKeyboardGeometry,
  );
});

test("keeps explicitly stored zero and other geometry values", () => {
  expect(
    touchKeyboardGeometryPreferences({
      touch_keyboard_height_adjustment: 0,
      touch_key_spacing_tenths: 35,
      touch_row_spacing_tenths: 95,
    }),
  ).toEqual({ heightAdjustment: 0, keySpacingTenths: 35, rowSpacingTenths: 95 });
});
