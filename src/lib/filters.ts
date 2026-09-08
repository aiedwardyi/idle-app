/**
 * Queue filters. `null` on an axis means no filter on that axis, which is not
 * the same as a filter that happens to match everything — an axis with one
 * option is not offered at all.
 *
 * In its own file rather than beside the screen so the screen stays a module
 * that only exports a component.
 */
export type Filters = {
  engine: string | null;
  status: string | null;
  folder: string | null;
};

export const NO_FILTERS: Filters = {
  engine: null,
  status: null,
  folder: null,
};
