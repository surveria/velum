var buffer = new ArrayBuffer(32);
var bytes = new Uint8Array(buffer);
var view = new DataView(buffer, 4, 24);

if (view.buffer !== buffer || view.byteOffset !== 4 || view.byteLength !== 24 ||
    view.constructor !== DataView || DataView.length !== 1 ||
    DataView.prototype[Symbol.toStringTag] !== "DataView") {
  throw new Test262Error("DataView metadata mismatch");
}

view.setInt8(0, 255);
view.setUint8(1, -1);
view.setInt16(2, -2);
view.setUint16(4, 0x1234, true);
view.setInt32(6, -2147483648, true);
view.setUint32(10, 0x89abcdef);
view.setFloat16(14, 1.337, true);
view.setFloat32(16, 1.5, true);
view.setFloat64(16, -Math.PI);

if (view.getInt8(0) !== -1 || view.getUint8(1) !== 255 ||
    view.getInt16(2) !== -2 || bytes[6] !== 255 || bytes[7] !== 254 ||
    view.getUint16(4, true) !== 0x1234 ||
    view.getInt32(6, true) !== -2147483648 ||
    view.getUint32(10) !== 0x89abcdef ||
    view.getFloat16(14, true) !== Math.f16round(1.337) ||
    view.getFloat64(16) !== -Math.PI) {
  throw new Test262Error("DataView numeric access mismatch");
}

var failures = 0;
try { DataView(buffer); } catch (error) {
  if (error instanceof TypeError) failures = failures + 1;
}
try { new DataView({}, 0); } catch (error) {
  if (error instanceof TypeError) failures = failures + 1;
}
try { new DataView(buffer, 31, 2); } catch (error) {
  if (error instanceof RangeError) failures = failures + 1;
}
try { view.getUint32(22); } catch (error) {
  if (error instanceof RangeError) failures = failures + 1;
}
try { DataView.prototype.getUint8.call({}); } catch (error) {
  if (error instanceof TypeError) failures = failures + 1;
}
if (failures !== 5) {
  throw new Test262Error("DataView error behavior mismatch");
}

42;
