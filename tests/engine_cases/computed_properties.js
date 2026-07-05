let camera = {
    name: "front-door",
    count: 40,
    nested: { value: 2 },
};

let key = "count";
let assigned = camera[key] = camera[key] + camera["nested"].value;
camera[1] = assigned;
camera[true] = camera["1"];

print(camera["name"], camera["missing"]);
print(assigned);
print(camera[true]);

camera[true]
