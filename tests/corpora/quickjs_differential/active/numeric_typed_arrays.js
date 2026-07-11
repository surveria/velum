var int8 = new Int8Array([127, 128, -129]);
var clamped = new Uint8ClampedArray([-1, 0.5, 1.5, 254.6, 300]);
var int16 = new Int16Array([32767, 32768, -32769]);
var uint16 = new Uint16Array([-1, 65537]);
var int32 = new Int32Array([2147483648, -2147483649]);
var uint32 = new Uint32Array([-1, 4294967297]);
var float32 = new Float32Array([1.337, Infinity, NaN]);
var float64 = new Float64Array([Math.PI, -0]);

print(int8[0], int8[1], int8[2]);
print(clamped[0], clamped[1], clamped[2], clamped[3], clamped[4]);
print(int16[0], int16[1], int16[2], uint16[0], uint16[1]);
print(int32[0], int32[1], uint32[0], uint32[1]);
print(float32[0] === Math.fround(1.337), float32[1] === Infinity,
  Number.isNaN(float32[2]), float64[0] === Math.PI, 1 / float64[1] === -Infinity);

var buffer = new ArrayBuffer(24);
var bytes = new Uint8Array(buffer);
var view16 = new Int16Array(buffer, 2, 3);
var view64 = new Float64Array(buffer, 8, 2);
view16[0] = -2;
view64[0] = Math.PI;
print(buffer.byteLength, view16.length, view16.byteLength, view16.byteOffset,
  view64.length, view64.byteLength, view64.byteOffset, bytes[2], bytes[3]);

print(Int8Array.name, Float64Array.name, Int16Array.BYTES_PER_ELEMENT,
  Float64Array.BYTES_PER_ELEMENT, Int16Array.prototype.BYTES_PER_ELEMENT);

try { new Int16Array(new ArrayBuffer(8), 1); } catch (error) {
  print("misaligned", error instanceof RangeError);
}
try { Int8Array(1); } catch (error) {
  print("typed-call", error instanceof TypeError);
}
try { ArrayBuffer(1); } catch (error) {
  print("buffer-call", error instanceof TypeError);
}
