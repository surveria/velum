var i = 0;
var total = 0;

while (i < 200) {
  var controls = "\b\f\n\r\t\v\0";
  if (controls === "\u0008\u000c\u000a\u000d\u0009\u000b\u0000") {
    total = total + 1;
  }

  var text = "\x41\u0042\u{43}";
  if (text === "ABC") {
    total = total + 1;
  }

  var continuation = "front\
door";
  if (continuation === "frontdoor") {
    total = total + 1;
  }

  i = i + 1;
}

undefined;
