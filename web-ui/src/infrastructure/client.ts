const apiServerUrl =
    process.env.NEXT_PUBLIC_API_SERVER_URL ||
    process.env.API_SERVER_URL ||
    'http://127.0.0.1:8080';

const client =  {
    get: async<T> (url: string): Promise<T> => {
        console.log("fetching", `${apiServerUrl}${url}`);
        const response = await fetch(`${apiServerUrl}${url}`);
        console.log("response", response);
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        return response.json() as Promise<T>;
    },

    post: async<T> (url: string, data: any): Promise<T> => {
        const response = await fetch(`${apiServerUrl}${url}`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(data),
        });
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        return response.json() as Promise<T>;
    }
}

export default client;
