export interface ProjectReference {name:string;url:string}

export const addonProjects = {
  dlss: [
    {name:"mochizuki0323 · RX 9000",url:"https://gitlab.com/mochizuki0323/DLSSNR-AMD"},
  ],
  daniel: [
    {name:"Daniel Blanco",url:"https://github.com/danielblnc/DLSS-NR-on-AMD"},
    {name:"AMDNR-Linux",url:"https://github.com/hassanm0301/AMDNR-Linux"},
  ],
  nexus: [{name:"Nexus Mods",url:"https://www.nexusmods.com/"}],
} as const;
