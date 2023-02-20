export type GameStatEntry = {
    SteamID: string;
    LastKnownSteamName: string;
    Hunter_Skeets: number;
    Witch_Crowns: number;
    Tongue_Cuts: number;
    Smoker_Self_Clears: number;
    Tank_Rocks_Skeeted: number;
    Hunter_High_Pounces_25: number;
    Death_Charges: number;
    Commons_Killed: number;
    Friendly_Fire_Done: number;
    Friendly_Fire_Received: number;
    Damage_Done_To_Survivors: number;
    Damage_Done_To_SI: number;
    Damage_Done_To_Tanks: number;
    profileUrl: string;
    avatarMediumSrc: string;
    Gameplay_Time: number;
    Commons_Killed_Per_Round_Average: number;
    Hunters_Skeeted_Per_Round_Average: number;
    Damage_Done_To_SI_Per_Round_Average: number;
    Friendly_Fire_Done_Per_Round_Average: number;
    Friendly_Recovers_Per_Round_Average: number;
    Total_Score: number;
    Place_In_Rank?: number;
};

export type AverageGameStatEntry = {
    id: number;
    SteamID: string;
    DateAdded: Date;
    Commons_Killed_In_Round_Entry: number;
    Hunters_Skeeted_In_Round_Entry: number;
    Damage_Done_To_SI_In_Round_Entry: number;
    Friendly_Fire_Done_In_Round_Entry: number;
    Friendly_Recover_In_Round_Entry: number;
};
