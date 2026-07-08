let raw = String.raw({ raw: { 0: "a", 1: "b", 2: "c", length: 3 } }, 1, 2);
let boxed = new String("Boxed");

print(String.fromCharCode(67, 97, 109));
print(String.fromCharCode(65.9, -1).length);
print(String.fromCodePoint(0x2603).codePointAt(0));
print(raw);
print("camera".at(-1), "cam".padStart(5, "0"), "cam".padEnd(6, "ab"));
print("\ttrim\n".trimLeft(), "\ttrim\n".trimRight());
print("MiXeD".toLocaleLowerCase(), "MiXeD".toLocaleUpperCase());
print(String.prototype.toString.call(boxed), String.prototype.valueOf.call(boxed));
print(String.prototype.trimLeft === String.prototype.trimStart);
