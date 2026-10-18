# A curve's turns between hemispheres

Animations (glTF), 2026-10-16.

## The original

Where a span's hemisphere turn differs from the previous span's, the original curve jumps at the key (1,242 keys of the converted files).

## Ours

The glTF curve follows the later span.

## Why

glTF curves are continuous; which side the game shows at the key is not known.
