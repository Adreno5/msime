import { expect, test } from "vitest";

test("shuangpin custom editor reuses shared mounted and generation lifecycles", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/shuangpin-custom-editor.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncActionRunner(");
  expect(source).toContain("useAsyncGeneration(");
  expect(source).not.toContain("const mounted = useRef(true)");
  expect(source).not.toContain("const generation = useRef(0)");
});
