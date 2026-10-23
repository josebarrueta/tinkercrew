import guardianData from "./treasure-guardian.v1.json";
import { loadMission } from "./definition";

export const treasureGuardian = loadMission(guardianData);
export const world = {
  name: "Maker Islands",
  stops: [
    { title: "Wake the Machine", missionId: null },
    { title: "LEGO Signal Tower", missionId: null },
    { title: "Curious Robot", missionId: null },
    { title: "Secret Control Panel", missionId: null },
    { title: treasureGuardian.title, missionId: treasureGuardian.id },
    { title: "Free Build Mode", missionId: "free-build" },
  ],
};
