import type { Notebook } from "./api";

export interface TreeNode extends Notebook {
  children: TreeNode[];
}

export type DropPosition = "before" | "inside" | "after";

/** Build a forest from the flat list (already sibling-ordered by the backend). */
export function buildTree(flat: Notebook[]): TreeNode[] {
  const byId = new Map<string, TreeNode>();
  for (const n of flat) byId.set(n.id, { ...n, children: [] });
  const roots: TreeNode[] = [];
  for (const node of byId.values()) {
    const parent = node.parentId ? byId.get(node.parentId) : undefined;
    (parent ? parent.children : roots).push(node);
  }
  return roots;
}

export function isDescendantOrSelf(
  flat: Notebook[],
  ancestorId: string,
  id: string,
): boolean {
  const parentOf = new Map(flat.map((n) => [n.id, n.parentId]));
  for (let cur: string | null | undefined = id; cur; cur = parentOf.get(cur)) {
    if (cur === ancestorId) return true;
  }
  return false;
}

/**
 * Translate "drop `dragId` before/inside/after `targetId`" into the
 * (newParentId, index) pair the backend expects. Returns null if the drop is invalid.
 * Index is relative to the target parent's children *excluding* the dragged item.
 */
export function computeMove(
  flat: Notebook[],
  dragId: string,
  targetId: string,
  pos: DropPosition,
): { newParentId: string | null; index: number } | null {
  if (isDescendantOrSelf(flat, dragId, targetId)) return null;
  const target = flat.find((n) => n.id === targetId);
  if (!target) return null;
  const childrenOf = (parentId: string | null) =>
    flat.filter((n) => n.parentId === parentId && n.id !== dragId);

  if (pos === "inside") {
    return { newParentId: targetId, index: childrenOf(targetId).length };
  }
  const siblings = childrenOf(target.parentId);
  const at = siblings.findIndex((n) => n.id === targetId);
  return {
    newParentId: target.parentId,
    index: pos === "before" ? at : at + 1,
  };
}
