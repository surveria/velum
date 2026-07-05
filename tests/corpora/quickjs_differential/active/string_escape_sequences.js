var controls = "\b\f\n\r\t\v\0";
var expectedControls = "\u0008\u000c\u000a\u000d\u0009\u000b\u0000";
var hex = "\x41\u0042\u{43}";
var quoted = "\"\'\\";
var continuation = "front\
door";

print(controls === expectedControls);
print(hex, quoted, continuation);
