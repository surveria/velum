let total = 0;
let index = 0;

while (index < 500) {
    let parsed = JSON.parse('{"camera":"front","active":true,"count":2,"items":[1,null,"x"],"nested":{"ok":false}}');
    let text = JSON.stringify({
        camera: parsed.camera,
        active: parsed.active,
        count: parsed.count,
        items: parsed.items,
        nested: parsed.nested,
        missing: undefined
    });
    total = total + parsed.count + text.length;
    index = index + 1;
}

print(total > 0);
