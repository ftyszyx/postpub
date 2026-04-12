import { createRouter, createWebHashHistory } from "vue-router";
import ArticlesView from "../views/ArticlesView.vue";
import ConfigView from "../views/ConfigView.vue";
import GenerationView from "../views/GenerationView.vue";
import OverviewView from "../views/OverviewView.vue";
import TaskStatusView from "../views/TaskStatusView.vue";
import TemplatesView from "../views/TemplatesView.vue";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      name: "overview",
      component: OverviewView,
      meta: { title: "Overview" }
    },
    {
      path: "/config",
      name: "config",
      component: ConfigView,
      meta: { title: "Config" }
    },
    {
      path: "/templates",
      name: "templates",
      component: TemplatesView,
      meta: { title: "Templates" }
    },
    {
      path: "/articles",
      name: "articles",
      component: ArticlesView,
      meta: { title: "Articles" }
    },
    {
      path: "/generation",
      name: "generation",
      component: GenerationView,
      meta: { title: "Generation" }
    },
    {
      path: "/tasks",
      name: "task-status",
      component: TaskStatusView,
      meta: { title: "Task Status" }
    }
  ]
});

export default router;
