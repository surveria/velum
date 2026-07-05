let total = 0;

for (let index = 0; index < 128; index++) {
    let text = String(index);
    total += text.length;

    let boxed = new String("camera");
    total += boxed.length;
    if (boxed[0] === "c") {
        total += 1;
    }
    if (boxed[5] === "a") {
        total += 1;
    }

    let keys = "";
    for (let key in "go") {
        keys = keys + key;
    }
    if (keys === "01") {
        total += 1;
    }
}

if (String(null) === "null") {
    total += 1;
}
if (String(undefined) === "undefined") {
    total += 1;
}
if (String(Object()) === "[object Object]") {
    total += 1;
}

total
