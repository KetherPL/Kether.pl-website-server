import SteamUser from 'steam-user';

const KETHER_GROUP_CHAT_ID = '6887767';
const KETHER_DEFAULT_CHAT_ID = '22190790';

let steamUser = new SteamUser({});

export const steamBot = {
    login: () => {
        try {
            steamUser.logOn({
                accountName: '',
                password: '',
                autoRelogin: true,
                rememberPassword: true,
            });
        } catch (error) {
            console.log("Couldn't login to steambot.");
            console.log(error);
        }
        // setTimeout(() => {
        //     steamUser.chat.setSessionActiveGroups(KETHER_GROUP_CHAT_ID);
        //     steamUser.chat.sendChatMessage(
        //         KETHER_GROUP_CHAT_ID,
        //         KETHER_DEFAULT_CHAT_ID,
        //         'Test hello from node js'
        //     );
        // }, 5000);
    },
    sendMessage: (message: string) => {
        try {
            steamUser.chat.sendChatMessage(
                KETHER_GROUP_CHAT_ID,
                KETHER_DEFAULT_CHAT_ID,
                message
            );
        } catch (error) {
            console.log("Couldn't send message to steam chat");
            console.log(error);
        }
    },
};
