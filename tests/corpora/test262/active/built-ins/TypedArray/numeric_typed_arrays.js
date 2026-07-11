var int8 = new Int8Array([127, 128, -129]);
if (int8[0] !== 127 || int8[1] !== -128 || int8[2] !== 127) {
  throw new Test262Error("Int8Array conversion mismatch");
}

var clamped = new Uint8ClampedArray([-1, 0.5, 1.5, 254.6, 300]);
if (clamped[0] !== 0 || clamped[1] !== 0 || clamped[2] !== 2 ||
    clamped[3] !== 255 || clamped[4] !== 255) {
  throw new Test262Error("Uint8ClampedArray conversion mismatch");
}

var int16 = new Int16Array([32767, 32768, -32769]);
var uint16 = new Uint16Array([-1, 65537]);
var int32 = new Int32Array([2147483648, -2147483649]);
var uint32 = new Uint32Array([-1, 4294967297]);
if (int16[1] !== -32768 || int16[2] !== 32767 ||
    uint16[0] !== 65535 || uint16[1] !== 1 ||
    int32[0] !== -2147483648 || int32[1] !== 2147483647 ||
    uint32[0] !== 4294967295 || uint32[1] !== 1) {
  throw new Test262Error("integer typed array conversion mismatch");
}

var float32 = new Float32Array([1.337, Infinity, NaN]);
var float64 = new Float64Array([Math.PI, -0]);
if (float32[0] !== Math.fround(1.337) || float32[1] !== Infinity ||
    !Number.isNaN(float32[2]) || float64[0] !== Math.PI ||
    1 / float64[1] !== -Infinity) {
  throw new Test262Error("floating-point typed array conversion mismatch");
}

var buffer = new ArrayBuffer(24);
var bytes = new Uint8Array(buffer);
var view16 = new Int16Array(buffer, 2, 3);
var view64 = new Float64Array(buffer, 8, 2);
view16[0] = -2;
view64[0] = Math.PI;
if (buffer.byteLength !== 24 || view16.length !== 3 ||
    view16.byteLength !== 6 || view16.byteOffset !== 2 ||
    view64.length !== 2 || view64.byteLength !== 16 ||
    view64.byteOffset !== 8 || bytes[2] !== 254 || bytes[3] !== 255) {
  throw new Test262Error("ArrayBuffer view mismatch");
}

var constructors = [
  Int8Array, Uint8Array, Uint8ClampedArray, Int16Array, Uint16Array,
  Int32Array, Uint32Array, Float32Array, Float64Array
];
var sizes = [1, 1, 1, 2, 2, 4, 4, 4, 8];
for (var index = 0; index < constructors.length; index = index + 1) {
  var constructor = constructors[index];
  if (constructor.length !== 1 || constructor.BYTES_PER_ELEMENT !== sizes[index] ||
      constructor.prototype.BYTES_PER_ELEMENT !== sizes[index]) {
    throw new Test262Error("typed array constructor metadata mismatch");
  }
}

var failures = 0;
try { new Int16Array(new ArrayBuffer(8), 1); } catch (error) {
  if (error instanceof RangeError) failures = failures + 1;
}
try { new Float64Array(new ArrayBuffer(8), 0, 2); } catch (error) {
  if (error instanceof RangeError) failures = failures + 1;
}
try { Int8Array(1); } catch (error) {
  if (error instanceof TypeError) failures = failures + 1;
}
try { ArrayBuffer(1); } catch (error) {
  if (error instanceof TypeError) failures = failures + 1;
}
if (failures !== 4) {
  throw new Test262Error("typed storage constructor error mismatch");
}

42;
