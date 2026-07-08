let text = "Camera Stream";
let padded = " \tCamera Stream\n ";
let boxed = new String("Boxed");

print(text.charAt(0), text.charAt(-1), text.charCodeAt(1), text.charCodeAt(99));
print(text.includes("Stream"), text.indexOf("a", 2), text.lastIndexOf("a", 2));
print(text.slice(1, 6), text.slice(-6, -1), text.substring(6, 0));
print(text.startsWith("mera", 2), text.endsWith("Camera", 6));
print("go".repeat(3), "a".concat("b", 7, true));
print(padded.trim(), padded.trimStart(), padded.trimEnd());
print("MiXeD".toLowerCase(), "MiXeD".toUpperCase());
print(boxed.slice(1, 4), String.prototype.slice.call(12345, 1, 4));
