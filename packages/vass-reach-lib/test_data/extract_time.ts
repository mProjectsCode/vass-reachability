const file = Bun.file("vass_6_5_15.txt");

const data = await file.text();
const points = data.split("SolverResult");
const time = points
    .filter((point) => !point.includes("Unknown"))
    .map((point) => {
        let regexp = /time: (\d*.\d*(?:ms|s))/;
        let match = point.match(regexp);
        if (match) {
            let time = match[1];
            console.log(`time: ${time}`);
            
            if (time.endsWith("ms")) {
                return parseFloat(time) / 1000;
            } else if (time.endsWith("s")) {
                return parseFloat(time);
            }
        }
        return undefined;
    })
    .filter((time) => time !== undefined)
    .map((time) => time as number);
let len = time.length;
let avgTime = time
    .reduce((acc, time) => acc + time, 0) / len;

console.log(`Average time: ${avgTime} seconds`);

export {};