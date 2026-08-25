import { customAlphabet } from "nanoid";

// Unambiguous alphabet (no 0/O/1/I/l) — scheme links get shared/typed, so
// typo-resistance matters more than raw entropy. 12 chars keeps a scheme
// effectively unguessable (the whole point of the no-accounts access model,
// see GOALS.md M6) while staying short enough to read aloud.
const alphabet = "23456789abcdefghjkmnpqrstuvwxyz";
const generate = customAlphabet(alphabet, 12);

export function generateSchemeSlug(): string {
  return generate();
}
