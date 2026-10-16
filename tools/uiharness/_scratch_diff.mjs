// scratch: where do two UI screenshots differ?
// usage: node _scratch_diff.mjs a.png b.png
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const [a, b] = process.argv.slice(2);
const py = `
import sys, struct, zlib
def load(p):
    d = open(p,'rb').read()
    pos = 8; w=h=None; idat=b''
    while pos < len(d):
        ln = struct.unpack('>I', d[pos:pos+4])[0]; typ = d[pos+4:pos+8]
        if typ == b'IHDR':
            w,h,bd,ct = struct.unpack('>IIBB', d[pos+8:pos+18])
        elif typ == b'IDAT':
            idat += d[pos+8:pos+8+ln]
        pos += 12 + ln
    raw = zlib.decompress(idat)
    ch = 4 if ct == 6 else 3
    stride = w*ch
    out = bytearray(w*h*ch)
    prev = bytearray(stride)
    i = 0
    for y in range(h):
        f = raw[i]; i += 1
        line = bytearray(raw[i:i+stride]); i += stride
        if f == 1:
            for x in range(ch, stride): line[x] = (line[x] + line[x-ch]) & 255
        elif f == 2:
            for x in range(stride): line[x] = (line[x] + prev[x]) & 255
        elif f == 3:
            for x in range(stride):
                left = line[x-ch] if x >= ch else 0
                line[x] = (line[x] + ((left + prev[x]) >> 1)) & 255
        elif f == 4:
            for x in range(stride):
                a_ = line[x-ch] if x >= ch else 0
                b_ = prev[x]; c_ = prev[x-ch] if x >= ch else 0
                p = a_ + b_ - c_
                pa, pb, pc = abs(p-a_), abs(p-b_), abs(p-c_)
                pr = a_ if (pa <= pb and pa <= pc) else (b_ if pb <= pc else c_)
                line[x] = (line[x] + pr) & 255
        out[y*stride:(y+1)*stride] = line
        prev = line
    return w, h, ch, out

wa, ha, ca, A = load(sys.argv[1])
wb, hb, cb, B = load(sys.argv[2])
print(f"a={wa}x{ha} ch={ca}  b={wb}x{hb} ch={cb}")
if (wa,ha) != (wb,hb):
    print("SIZE MISMATCH"); sys.exit(0)
cols = [0]*wa
rows = [0]*ha
diff = 0
for y in range(ha):
    for x in range(wa):
        ia = (y*wa+x)*ca; ib = (y*wb+x)*cb
        if abs(A[ia]-B[ib]) + abs(A[ia+1]-B[ib+1]) + abs(A[ia+2]-B[ib+2]) > 24:
            cols[x] += 1; rows[y] += 1; diff += 1
tot = wa*ha
print(f"differing pixels: {diff} / {tot} = {100.0*diff/tot:.2f}%")
first_col = next((x for x in range(wa) if cols[x] > ha*0.02), None)
print("first column with >2% differing pixels:", first_col)
print("first row with >2% differing pixels:", next((y for y in range(ha) if rows[y] > wa*0.02), None))
band = [(x, cols[x]) for x in range(0, 400, 20)]
print("column diff profile x=0..400:", band)
`;
writeFileSync("_scratch_diff.py", py);
console.log(execFileSync("python", ["_scratch_diff.py", a, b], { encoding: "utf8" }));
