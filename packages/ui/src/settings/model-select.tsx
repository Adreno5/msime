export interface ModelSelectProps {
  models: readonly string[];
  model: string;
  ariaLabel: string;
  emptyLabel: string;
  onSelect: (model: string) => void;
}

/** Selects a known model while leaving custom values editable in the companion input. */
export function ModelSelect({ models, model, ariaLabel, emptyLabel, onSelect }: ModelSelectProps) {
  return (
    <ControlledSelect
      aria-label={ariaLabel}
      value={models.includes(model) ? model : ""}
      onChange={(value) => {
        if (value) onSelect(value);
      }}
    >
      <option value="">{emptyLabel}</option>
      {models.map((entry) => (
        <option key={entry} value={entry}>
          {entry}
        </option>
      ))}
    </ControlledSelect>
  );
}
import { ControlledSelect } from "../core/controlled-select";
