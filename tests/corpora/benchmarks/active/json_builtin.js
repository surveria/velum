let total = 0;
let index = 0;

while (index < 500) {
    let parsed = JSON.parse('{"camera":"front","active":true,"count":2,"items":[1,null,"x"],"nested":{"ok":false}}');
    let boxedNumber = new Number(10);
    boxedNumber.toString = function() {
        return "toString";
    };
    let boxedSpace = new Number(2);
    boxedSpace.valueOf = function() {
        return 2;
    };
    let text = JSON.stringify({
        camera: parsed.camera,
        active: parsed.active,
        count: parsed.count,
        boxed: new Boolean(true),
        items: parsed.items,
        nested: parsed.nested,
        missing: undefined
    }, [boxedNumber, "camera", "active", "count", "boxed", "items", "nested"], boxedSpace);
    total = total + parsed.count + text.length + JSON.stringify(new Number(index)).length;
    index = index + 1;
}

total > 0;
