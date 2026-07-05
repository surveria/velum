let camera = {
    name: "front-door",
    count: 40,
    nested: { value: 2 },
    duplicate: 1,
    duplicate: 41,
};

let assigned = camera.count = camera.count + camera.nested.value;
camera.extra = camera.duplicate + 1;

let shared = {};
let same = shared === shared;
let different = shared === {};

print(camera.name, camera.missing);
print(assigned);
print(camera.extra);

same && !different ? camera.count : 0
