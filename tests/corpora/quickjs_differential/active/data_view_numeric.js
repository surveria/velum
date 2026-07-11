var buffer = new ArrayBuffer(32);
var bytes = new Uint8Array(buffer);
var view = new DataView(buffer, 4, 24);

view.setInt8(0, 255);
view.setUint8(1, -1);
view.setInt16(2, -2);
view.setUint16(4, 0x1234, true);
view.setInt32(6, -2147483648, true);
view.setUint32(10, 0x89abcdef);
view.setFloat32(14, 1.5, true);
view.setFloat64(16, -Math.PI);

print(DataView.name, DataView.length, view.byteOffset, view.byteLength,
  view.buffer === buffer, DataView.prototype[Symbol.toStringTag]);
print(view.getInt8(0), view.getUint8(1), view.getInt16(2), bytes[6], bytes[7]);
print(view.getUint16(4, true), view.getInt32(6, true), view.getUint32(10));
print(view.getFloat32(14, true), view.getFloat64(16));

try { DataView(buffer); } catch (error) {
  print("call", error instanceof TypeError);
}
try { view.getUint32(22); } catch (error) {
  print("range", error instanceof RangeError);
}
