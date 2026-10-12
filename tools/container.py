"""Reader for the serialized-heap container used by .map/.gam/.ls/.cin/.loc.

Format notes: notes/formats.md#container
  - file starts AF FA AF FA, u32 version (3)
  - pointer  = BB BB BB BB, u32 absolute file offset, u32 element count
  - every pointer target starts with the guard word BE BE BE BE
  - file ends with EF FE EF FE
"""
import struct

MAGIC = b'\xaf\xfa\xaf\xfa'
PTR = b'\xbb\xbb\xbb\xbb'
GUARD = b'\xbe\xbe\xbe\xbe'


class Container:
    def __init__(self, data):
        if data[:4] != MAGIC:
            raise ValueError('not an AFFA container')
        self.data = data
        self.version = struct.unpack_from('<I', data, 4)[0]
        self.pointers = []          # (pointer offset, target offset, count)
        self.refs = {}              # target offset -> [pointer offsets]
        self._scan()

    @classmethod
    def load(cls, path):
        with open(path, 'rb') as f:
            return cls(f.read())

    def _scan(self):
        d = self.data
        i = 0
        while True:
            i = d.find(PTR, i)
            if i < 0:
                break
            off, cnt = struct.unpack_from('<II', d, i + 4)
            if off < len(d) and d[off:off + 4] == GUARD:
                self.pointers.append((i, off, cnt))
                self.refs.setdefault(off, []).append(i)
            i += 4

    def allocations(self):
        """Yield (offset, size, count); offset is the guard, size includes it."""
        targets = sorted(self.refs)
        counts = {off: cnt for _, off, cnt in self.pointers}
        for a, b in zip(targets, targets[1:]):
            yield a, b - a, counts[a]

    def name_near(self, ptr_offset, window=0x80):
        """Best-effort: the nearest preceding length-prefixed (u32 0x14) name.

        Owning objects store a 20-byte name field shortly before their pointers.
        """
        d = self.data
        lo = max(0, ptr_offset - window)
        j = d.rfind(b'\x14\x00\x00\x00', lo, ptr_offset)
        while j >= 0:
            raw = d[j + 4:j + 24].split(b'\0')[0]
            if raw and all(32 < c < 127 for c in raw):
                return raw.decode('ascii')
            j = d.rfind(b'\x14\x00\x00\x00', lo, j)
        return None
