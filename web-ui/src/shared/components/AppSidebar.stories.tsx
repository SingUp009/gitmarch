import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { AppSidebar } from "./AppSidebar";

const meta = {
  component: AppSidebar,
  parameters: {
    layout: "fullscreen",
  },
} satisfies Meta<typeof AppSidebar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Dashboard: Story = {
  parameters: {
    nextjs: { navigation: { pathname: "/" } },
  },
};

export const Repositories: Story = {
  parameters: {
    nextjs: { navigation: { pathname: "/repos" } },
  },
};

export const Users: Story = {
  parameters: {
    nextjs: { navigation: { pathname: "/users" } },
  },
};

export const Settings: Story = {
  parameters: {
    nextjs: { navigation: { pathname: "/settings" } },
  },
};
