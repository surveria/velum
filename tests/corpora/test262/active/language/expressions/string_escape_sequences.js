let value = 0;
let controls = "\b\f\n\r\t\v\0";
let expectedControls = "\u0008\u000c\u000a\u000d\u0009\u000b\u0000";
let hex = "\x41\u0042\u{43}";
let quoted = "\"\'\\";
let continuation = "front\
door";

if (controls !== expectedControls) {
  throw new Test262Error("control escape mismatch");
}
if (hex !== "ABC") {
  throw new Test262Error("hex or unicode escape mismatch");
}
if (quoted !== "\"'\\") {
  throw new Test262Error("quoted escape mismatch");
}
if (continuation !== "frontdoor") {
  throw new Test262Error("line continuation mismatch");
}

value = 42;
value;
