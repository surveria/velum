let values = [40, 1, 2];
let assigned = values[1] = values[0] + values[2];
values[3] = assigned;
values["01"] = 7;

let empty = [];
let trailing = [40, 2,];

print(values.length, values[2], values[9]);
print(values["01"], values.length);
print(empty.length, trailing.length);

values.length + values[3] - 4
