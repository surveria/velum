let value = 0;
let controls = "\b\f\n\r\t\v\0";
let expectedControls = "\u0008\u000c\u000a\u000d\u0009\u000b\u0000";
let hex = "\x41\u0042\u{43}";
let quoted = "\"\'\\";
let continuation = "front\
door";

if (controls === expectedControls) {
  value = value + 10;
}
if (hex === "ABC") {
  value = value + 10;
}
if (quoted === "\"'\\") {
  value = value + 10;
}
if (continuation === "frontdoor") {
  value = value + 12;
}

print(hex, quoted, continuation);
value;
