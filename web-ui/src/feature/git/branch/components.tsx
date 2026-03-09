"use client";

import { useEffect, useState } from "react";
import { Branch, getBranches } from "./branch";
import { switchBranch } from "../switch/switch";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/shared/components/ui/select";

type Props = {
    repository: string;
}

export const BranchList = (props: Props) => {
    const [branches, setBranches] = useState<Branch[]>([]);

    const handleBranchSelect = async (branchName: string) => {
        try {
            await switchBranch(props.repository, branchName);
        } catch (error) {
            console.error("Failed to checkout branch", error);
            alert(`Failed to checkout branch: ${error instanceof Error ? error.message : String(error)}`);
        }
    }

    useEffect(() => {
        let isMounted = true;
        getBranches(props.repository)
            .then((items) => {
                if (isMounted) {
                    setBranches(items);
                }
            })
            .catch((error) => {
                console.error("failed to load branches", error);
                if (isMounted) {
                    setBranches([]);
                }
            });

        return () => {
            isMounted = false;
        };
    }, [props.repository]);

    return (
        <Select onValueChange={handleBranchSelect}>
            <SelectTrigger>
                <SelectValue />
            </SelectTrigger>
            <SelectContent>
                <SelectGroup>
                    {branches.map((branch) => (
                        <SelectItem key={branch.name} value={branch.name}>
                            {branch.name}
                        </SelectItem>
                    ))}
                </SelectGroup>
            </SelectContent>
        </Select>
    );
}
